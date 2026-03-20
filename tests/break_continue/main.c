int main() {
    int sum = 0;
    int i = 0;
    while (i < 20) {
        if (i == 10) {
            break;
        }
        if (i % 2 == 0) {
            i = i + 1;
            continue;
        }
        sum = sum + i;
        i = i + 1;
    }
    return sum;
}
