int main() {
    int result = 0;
    if (1 && 1) result = result + 1;
    if (1 || 0) result = result + 1;
    if (!(0)) result = result + 1;
    return result;
}
