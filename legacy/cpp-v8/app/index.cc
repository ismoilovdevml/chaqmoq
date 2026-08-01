#include "v8.h"

#include "./src/chaqmoq.hpp"

int main(int argc, char *argv[])
{
    char *filename = argv[1];
    auto *chaqmoq = new Chaqmoq();
    std::unique_ptr<v8::Platform> platform =
        chaqmoq->initializeV8(argc, argv);

    chaqmoq->initializeVM();
    chaqmoq->InitializeProgram(filename);
    chaqmoq->Shutdown();

    return 0;
}