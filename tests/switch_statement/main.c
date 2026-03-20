int printf(const char *fmt, ...);

int main() {
    int x = 2;
    switch (x) {
        case 1: printf("one\n"); break;
        case 2: printf("two\n"); break;
        case 3: printf("three\n"); break;
        default: printf("other\n"); break;
    }

    /* Test default case */
    switch (99) {
        case 1: printf("bad\n"); break;
        default: printf("default\n"); break;
    }

    return 0;
}
