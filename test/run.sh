set -euo pipefail

cd $(dirname $0)

ARUFA="${ARUFA:-../target/debug/arufa}"
CC="${CC:-gcc}"
DIR="${DIR:-$(mktemp -d)}"

assert() {
    local name="$1"
    local path="$2"
    local expected="$3"
    local actual=0

    local ir_path=$DIR/${path%.*}.ll
    local obj_path=$DIR/${path%.*}.o
    local bin_path=$DIR/${path%.*}

    $ARUFA --emit ir $path $ir_path
    $ARUFA --emit obj $path $obj_path
    $CC $obj_path -o $bin_path
    $bin_path || actual=$?
    if [ $actual -eq $expected ]; then
        echo "[${name}] OK"
    else
        echo "[${name}] Error: expected ${expected}, actual ${actual}"
    fi
}


assert "integer literal" ./int.arufa 42
assert "binary expression" ./bin.arufa 13
assert "variable" ./var.arufa 95
