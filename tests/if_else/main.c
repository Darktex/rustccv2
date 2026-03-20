int printf(const char *fmt, ...);

int main() {
    int x = 10;
    if (x > 5) {
        printf("x is big\n");
    } else {
        printf("x is small\n");
    }
    return 0;
}
