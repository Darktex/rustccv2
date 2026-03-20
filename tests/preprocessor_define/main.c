int printf(const char *fmt, ...);

#define VALUE 42
#define DOUBLE(x) ((x) * 2)
#define ADD(a, b) ((a) + (b))

int main() {
    printf("%d\n", VALUE);
    printf("%d\n", DOUBLE(10));
    printf("%d\n", ADD(3, 4));
    return 0;
}
