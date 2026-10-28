#include <stdio.h>

void print(char* str) {
    fputs(str, stdout);
}

void print_i32(int i) {
    printf("%d", i);
}

void print_u32(unsigned int i) {
    printf("%d", i);
}

int add_c(int a, int b) {
    return a + b;
}

unsigned int fact_c(unsigned int n) {
    if (n == 1) {
        return 1;
    } else {
        return n * fact_c(n - 1);
    }
}

int sum_array_c(int array[5]) {
    return array[0] + array[1] + array[2] + array[3] + array[4];
}
