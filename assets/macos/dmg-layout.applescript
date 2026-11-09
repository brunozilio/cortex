-- Only the mounted image is changed; the user's Finder preferences stay intact.
on run argv
    set mountPath to item 1 of argv
    set mountedFolder to POSIX file mountPath as alias
    set artwork to POSIX file (mountPath & "/.background/background.png") as alias
    with timeout of 60 seconds
        tell application "Finder"
            set installerWindow to make new Finder window to mountedFolder
            set current view of installerWindow to icon view
            set toolbar visible of installerWindow to false
            set statusbar visible of installerWindow to false
            set bounds of installerWindow to {160, 120, 820, 568}
            set viewOptions to icon view options of installerWindow
            set arrangement of viewOptions to not arranged
            set icon size of viewOptions to 128
            set text size of viewOptions to 13
            set label position of viewOptions to bottom
            set background picture of viewOptions to artwork
            set position of item "Superfik.app" of installerWindow to {170, 210}
            set position of item "Applications" of installerWindow to {490, 210}
            close installerWindow
            set installerWindow to make new Finder window to mountedFolder
            update mountedFolder without registering applications
            delay 2
            close installerWindow
        end tell
    end timeout
end run
