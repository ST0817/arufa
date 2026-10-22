set -euo pipefail

cd $(dirname $0)

ARUFA="${ARUFA:-../target/debug/arufa}"
CC="${CC:-gcc}"
DIR="${DIR:-$(mktemp -d)}"

$ARUFA --emit obj ./int.arufa ./int.o
$CC ./int.o -o ./int
./int || actual=$?
if [ $actual -eq 42 ]; then
    echo "OK"
else
    echo "Error: actual ${actual}"
fi
