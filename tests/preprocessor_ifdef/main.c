int printf(const char *fmt, ...);

#define FEATURE_A

int main() {
#ifdef FEATURE_A
    printf("A\n");
#else
    printf("no A\n");
#endif

#ifndef FEATURE_B
    printf("no B\n");
#else
    printf("B\n");
#endif

#define VERSION 2
#if VERSION == 1
    printf("v1\n");
#elif VERSION == 2
    printf("v2\n");
#else
    printf("unknown\n");
#endif

    return 0;
}
