set -euo pipefail

cd $(dirname $0)

ARUFA="${ARUFA:-../target/debug/arufa}"
CC="${CC:-gcc}"
DIR="${DIR:-$(mktemp -d)}"

$ARUFA --emit ir ./test.arufa $DIR/test.ll
$ARUFA --emit obj ./test.arufa $DIR/test.o
$CC -g $DIR/test.o ./test.c -o $DIR/test
$DIR/test
