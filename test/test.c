#include <stdio.h>

void print(char* str) {
    fputs(str, stdout);
}

void iprint(int i) {
    printf("%d", i);
}

int add_c(int a, int b) {
    return a + b;
}

int fact_c(int n) {
    if (n == 1) {
        return 1;
    } else {
        return n * fact_c(n - 1);
    }
}
