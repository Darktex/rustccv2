int main() {
    int sum = 0;
    for (int i = 0; i < 20; i++) {
        if (i == 10) break;
        if (i % 2 == 0) continue;
        sum += i;
    }
    return sum;
}
