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
    // Os bilan events bo'lgan interestni register qiladigan event registrator
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
    // binzing thereadslarimiz uchun handle threadpool
    thread_pool: Vec<NodeThread>,
    // Barcha timerlarimizni va ularning muddati tugashi bilan
    // callback qilish uchun idenfikatorni hold qilib turadi
    timers: BTreeMap<Instant, usize>,
    // timerlarni remove qilish uchun va vaqtincha hold qilib turishi uchun struct
    // Biz runtimega ruxsat beramiz
    // ownership shuning uchun biz uni qayta ishlatishimiz mumkin bo'lgan same meory
    timers_to_remove: Vec<Instant>,
}