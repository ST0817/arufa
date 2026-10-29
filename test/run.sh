set -euo pipefail

cd $(dirname $0)

ARUFA="${ARUFA:-../target/debug/arufa}"
LLC="${LLC:-llc}"
CC="${CC:-clang}"
DIR="${DIR:-$(mktemp -d)}"

$ARUFA --emit ir ./test.arufa $DIR/test-llvm.ll
$LLC $DIR/test-llvm.ll -filetype=obj -relocation-model=pic -o $DIR/test-llvm.o
$CC -g $DIR/test-llvm.o ./test.c -o $DIR/test-llvm
$DIR/test-llvm

$ARUFA --emit asm ./test.arufa $DIR/test-asm.s
$CC -g $DIR/test-asm.s ./test.c -o $DIR/test-asm
$DIR/test-asm

$ARUFA --emit obj ./test.arufa $DIR/test.o
$CC -g $DIR/test.o ./test.c -o $DIR/test
$DIR/test
