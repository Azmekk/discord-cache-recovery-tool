package main

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"runtime"

	"github.com/h2non/filetype"
)

// cachedFileData holds the detected file type and the byte offset where the
// actual media content begins. startingByte is the offset past the Chromium
// Simple Cache header; on Windows it is always 0 because the filetype library
// matches directly at offset 0.
type cachedFileData struct {
	fileExtension string
	startingByte  int
}

// knownUnreadableFiles lists Chromium Simple Cache index and block files.
// These are internal bookkeeping files for the cache engine, not actual
// cached content, so we skip them.
var knownUnreadableFiles = [...]string{"index", "data_0", "data_1", "data_2", "data_3"}

var outputDir string = getOutputDir()

var errUnknownFiletype = errors.New("unknown filetype")

func readAndSeparateFile(dirEntry os.DirEntry, discordCacheFolder string) {
	if dirEntry.IsDir() {
		return
	}

	for _, item := range knownUnreadableFiles {
		if item == dirEntry.Name() {
			return
		}
	}

	filePath := filepath.Join(discordCacheFolder, dirEntry.Name())

	buffer, err := os.ReadFile(filePath)
	if err != nil {
		fmt.Println("Could not read file: ", dirEntry.Name())
		return
	}

	fileData, err := getNewFileData(buffer)

	if errors.Is(err, errUnknownFiletype) {
		return
	}

	if err != nil {
		fmt.Println("Encountered error while trying to determine file type for file: ", dirEntry.Name())
		return
	}

	saveDir := getOrCreateSaveDir(outputDir, fileData.fileExtension)

	buffer = buffer[fileData.startingByte:]
	fileExists, newFilePath := getNewFilePath(buffer, saveDir, dirEntry.Name(), fileData.fileExtension)

	if fileExists {
		return
	}

	err = os.WriteFile(newFilePath, buffer, 0644)
	if err != nil {
		fmt.Println("Error writing file: ", filePath, " due to err: ", err)
		return
	}
}

func fileNameExists(filePath string) bool {
	_, err := os.Stat(filePath)
	return !os.IsNotExist(err)
}

func isSameFile(buffer []byte, existingFilePath string) bool {
	fileData, err := os.ReadFile(existingFilePath)
	if err != nil {
		fmt.Println("Error reading file: ", existingFilePath, "to compare with buffer due to err: ", err)
		return true
	}

	return bytes.Equal(fileData, buffer)
}

func getNewFilePath(buffer []byte, saveDir string, fileName string, fileExtension string) (bool, string) {
	filePath := filepath.Join(saveDir, fmt.Sprintf("%s.%s", fileName, fileExtension))

	if fileNameExists(filePath) && isSameFile(buffer, filePath) {
		return true, ""
	}

	i := 0
	for fileNameExists(filePath) {
		i++
		filePath = filepath.Join(saveDir, fmt.Sprintf("%d_%s.%s", i, fileName, fileExtension))
	}

	return false, filePath
}

func getOutputDir() string {
	exePath, err := os.Executable()
	if err != nil {
		fmt.Println("Error retrieving executable path: ", err)
		os.Exit(1)
	}

	outputDir := filepath.Join(filepath.Dir(exePath), "discord_cache_output")
	if err := os.MkdirAll(outputDir, 0755); err != nil {
		fmt.Println("Error creating output directory: ", err)
		os.Exit(1)
	}

	return outputDir
}

func getOrCreateSaveDir(baseDir string, fileExtension string) string {
	saveDir := filepath.Join(baseDir, fileExtension)

	if err := os.MkdirAll(saveDir, 0755); err != nil {
		fmt.Println("Error:", err)
	}

	return saveDir
}

func getDiscordCacheFolderBasedOnOS() string {
	operatingSystem := runtime.GOOS
	switch operatingSystem {
	case "windows":
		userConfigDir, err := os.UserConfigDir()
		if err != nil {
			fmt.Println("Something went wrong when grabbing the discord cache directory:", err)
			os.Exit(1)
			return ""
		}
		return filepath.Join(userConfigDir, "discord/Cache/Cache_Data")
	case "darwin":
		userHomeDir, err := os.UserHomeDir()
		if err != nil {
			fmt.Println("Something went wrong when grabbing the discord cache directory:", err)
			os.Exit(1)
			return ""
		}
		return filepath.Join(userHomeDir, "Library/Application Support/discord/Cache/Cache_Data")
	case "linux":
		userHomeDir, err := os.UserHomeDir()
		if err != nil {
			fmt.Println("Something went wrong when grabbing the discord cache directory:", err)
			os.Exit(1)
			return ""
		}
		return filepath.Join(userHomeDir, ".config/discord/Cache/Cache_Data")
	default:
		fmt.Println("Unrecognized OS")
		os.Exit(1)
		return ""
	}
}

func getNewFileData(buffer []byte) (cachedFileData, error) {
	if runtime.GOOS == "windows" {
		return getFileExtension(buffer)
	} else {
		return getFileExtensionLinux(buffer)
	}
}

// getFileExtension detects the file type on Windows. The filetype library
// matches magic bytes directly at offset 0, so no scanning is needed.
func getFileExtension(buffer []byte) (cachedFileData, error) {
	fileInfo, _ := filetype.Match(buffer)
	if fileInfo == filetype.Unknown {
		return cachedFileData{}, errUnknownFiletype
	}

	return cachedFileData{fileInfo.Extension, 0}, nil
}

// getFileExtensionLinux detects the file type on Linux and macOS.
// Discord/Electron uses Chromium's Simple Cache format, where each entry
// starts with a SimpleFileHeader (magic number, version, key hash, URL key).
// The actual file content starts at a variable offset, so we scan the first
// 70 bytes looking for a recognized file-type magic signature.
func getFileExtensionLinux(buffer []byte) (cachedFileData, error) {
	for i := 0; i < 70; i++ {
		if i >= len(buffer) {
			break
		}

		fileInfo, _ := filetype.Match(buffer[i:])

		if fileInfo == filetype.Unknown {
			continue
		}

		return cachedFileData{fileInfo.Extension, i}, nil
	}

	return cachedFileData{}, errUnknownFiletype
}
