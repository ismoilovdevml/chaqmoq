
static mut RUNTIME: *mut Runtime = std::ptr::null_mut();

struct task {
    task: Box<dyn Fn() -> Js + Send + 'static>,
    callback_id: usize,
    kind: ThreadPollTaskKind,
}

impl Tasks {
    fn close() -> Self {
        Task {
            task: Box::new(|| Js::Underfined),
            callback_id: 0,
            kind: ThreadPollTaskKind::Close, 
        }
    }
}
// ThreadPoolTaskKind

pub enum ThreadPollTaskKind {
    FileRead,
    Encrypt,
    Close,
}

// JavaScript JS
#[derive(Debug)]
pub enum Js {
    Underfined,
    String(String),
    Int(usize),
}

impl Js {
    // Turlarni bilganimiz uchun qulaylik methodi
    fn into_string(self) -> Option<String> {
        match self {
            Js::String(s) => Some(s),
            _ => None,
        }
    }
    // Turlarni bilganimiz uchun qulaylik methodi
    fn into_int(self) -> Option<String> {
        match self {
            Js::String(s) => Some(n),
            _ => None,
        }
    }
}

// NodeThread
#[derive(Debug)]
struct NodeThread {
    pub(crate) handle: JoinHandle<()>,
    sender: Sender<Task>,
}




pub struct Runtime {
    // Mavjud threadlar uchun threadpool
    available_threads: Vec<usize>,
    // Callbacks schedule run
    callbacks_to_run: Vec<(usize, Js)>,
    // Barcha ro'yxatdan o'tgan callbacklar
    callback_queue: HashMap<usize, Box<dyn FnOnce(Js)>>,
    // Kutilayotgan epool eventslar soni (faqat print uchun ishlatamiz)
    epool_pending_events: usize,
    // OS bilan events bo'lgan interestni register qiladigan event registrator
    epool_registrator: minimio::Registrator,
    // epool thread uchun handle
    epool_thread: thread::JoinHandle<()>,
    epool_timeout: Arc<Mutex<Option<i32>>>,
    //events send uchun channel va epool thread tomonidan foydalaniladi
    // main loop
    event_receiver: Receiver<PoolEvent>,
    // bizning callbacklarimiz uchun unique identify yaratadi
    identity_token: usize,
    // kutilayotgan events pending bu 0 ga teng bo'lganda biz tugatamiz
    pedding_events: usize,
    // bizning thereadslarimiz uchun handle threadpool
    thread_pool: Vec<NodeThread>,
    // Barcha timerlarimizni va ularning muddati tugashi bilan
    // callback qilish uchun idenfikatorni hold qilib turadi
    timers: BTreeMap<Instant, usize>,
    // timerlarni remove qilish uchun va vaqtincha hold qilib turishi uchun struct
    // Biz runtimega ruxsat beramiz
    // ownership shuning uchun biz uni qayta ishlatishimiz mumkin bo'lgan same memory
    timers_to_remove: Vec<Instant>,
}

// bizning epoll-eventloop shug'ullanadigan 3ta eventsni tavsiflaydi
enum PoolEvent {
    // Threadpooldan thread idni olgan tuple containing events
    // callback_id va biz qayta ishlashni kutayotgan ma'lumotlar
    // callback
    Threadpool((usize, usize, Js)),
    //event_id ga ega bo'lgan epool-based eventloop events
    // event
    Epool(usize),
    Timeout,
}

impl Runtime {
    pub fn new() -> Self{
        // ===== DOIMIY THREADPOOL =====
        let (event_sender, event_receiver) = channel::<PoolEvent>();
        let mut threads = Vec::with_capacity(4);


        for i in 0..4 {
            let (event_sender, event_receiver) = channel::<Task>();
            let event_sender = event_sender.clone();

            let handle = thread::Builder::new()
                .name(format!("pool{}", i))
                .spawn(move || {
                    while let Ok(task) = evt_receiver.recv(){
                        println!(format!("quyidagi turdagi task olindi: {}", task.kind));

                        if let ThreadPollTaskKind::Close = task.kind {
                            break;
                        };

                        let res = (task.task)();
                        println!(format!("{} turdagi tackni tugatdi", task.kind));

                        let event = PoolEvent::Threadpool((i, task.callback_id, res));
                        event_sender.send(event).expect("threadpool");
                    }
                })
                .expect("Threapoolni ishga tushirib bo'lmadi");
            
            let node_thread = NodeThread {
                handle,
                sender: evt_sender,
            };

            threads.push(node_thread);
        }
    }
    pub fn run(mut self, f: impl Fn()) {
        let rt_ptr: *mut Runtime = &mut self;
        unsafe { RUNTIME = rt_ptr };

        let mut ticks = 0; //faqat print qilish uchun
        // avval main funksiyani ishga tushiramiz
        f();

        // ===== EVENT LOOP =====

        while self.pending_events > 0 {
            ticks += 1;

            print(format!("===== TICK {} =====", ticks));

            // ===== TIMERS =====
            self.process_expried_timers();

            // ===== CALLBACK =====
            self.run_callbacks();

            // ====== POOL =====

            // avval biz events bor yoki yo'qligin tekshiramiz kerak
            // agar bo'lmasa break qilamiz

            if self.pending_events == 0 {
                break;
            }
            // biz keyinga timeoutgavaqt olishni xoxlaymiz 
            // epool waitni keyingi timer uchun vaqt tugashi bilan
            // bir qilib qo'ydik
            let next_timeout = self.get_next_timer();

            let mut epoll_timeout_lock = self.epoll_timeout.lock().unwrap();
            *epoll_timeout_lock = next_timeout;
            // recv oldin biz lockni bo'shatamiz
            drop(epoll_timeout_lock);
            // biz bitta bitta evetni ko'rib chiqamiz 
            if let OK(event) = self.event_receiver.recv(){
                match event {
                    PoolEvent::Timeout => (),
                    PoolEvent::Threadpool((thread_id, callback_id, data)) => {
                        self.process_threadpool_events(thread_id, callback_id, data);
                    }
                    PoolEvent::Epool(event_id) => {
                        self.process_epool_events(event_id);
                    }
                }
            }
            self.run_callbacks();
            // ===== TEKSHIRISH =====
            // o'rnatilgan immidiate funksiyani oson qo'shishimiz mumkin
            //lekin biz buni bu yerda qilmaymiz


            // ====== Callbacklarni yopish ======
            // resurslarni bo'shatish extemsionlarimiz uchun yana bir hook
            // buning o'rniga biz har bir callbackga resurlarni chaqiramiz


            // biz barca resourcelarni tozalaymiz
            // destructorlarni ishlashiga qaraymiz

            for thread in self.thread_pool.ito_iter() {
                thread.sender.send(Task::close()).expect("threadpoolni tozalash");
                thread.handle.join().unwrap();
            }

            self.epoll_registrator.close_loop().unwrap();
            self.epoll_thread.join().unwrap();

            print("TUGADI")
        }
    }
}