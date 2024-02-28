echo "Setting the target architecture to amd64"
$env:GOARCH = "amd64"

echo "Setting the target operating system to windows"
$env:GOOS = "windows"
echo "Building for windows x86-64"
go build -o ./bin/recoverdiscordcache_windows.exe

echo "Setting the target operating system to linux"
$env:GOOS = "linux"
echo "Building for linux x86-64"
go build -o ./bin/recoverdiscordcache_linux

echo "Setting the target operating system to darwin (macOS)"
$env:GOOS = "darwin"
echo "Building for mac x86-64"
go build -o ./bin/recoverdiscordcache_mac

echo "Resetting the environment variables"
Remove-Item Env:\GOARCH
Remove-Item Env:\GOOS

echo "Script execution completed."