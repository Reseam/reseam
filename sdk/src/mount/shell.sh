app_path() { pm path "$1" | head -n 1 | sed 's/^package://'; }
app_dir() { dirname "$(app_path "$1")"; }
version_code() { dumpsys package "$1" | sed -n 's/.*versionCode=\([0-9]*\).*/\1/p' | head -n 1; }
same_file() { [ "$(stat -c %d:%i "$1")" = "$(stat -c %d:%i "$2")" ]; }
mounted() {
    [ -d "$2/apk" ] || return 1
    local app file
    app=$(app_dir "$1")
    for file in "$2"/apk/*; do
        same_file "$app/${file##*/}" "$file" || return 1
    done
}
unmount() {
    local app file
    app=$(app_dir "$1")
    for file in "$2"/apk/*; do
        while [ -f "$file" ] && same_file "$app/${file##*/}" "$file"; do umount -l "$app/${file##*/}"; done
    done
}
recompile() { pm compile --reset "$1" && pm compile -r install "$1"; }
