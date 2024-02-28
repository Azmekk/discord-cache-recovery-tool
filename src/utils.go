package main

import (
	"bytes"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"runtime"

	"github.com/h2non/filetype"
)

type cachedFileData struct {
	fileExtension string
	startingByte  int
}

var knownUnreadableFiles = [...]string{"index", "data_0", "data_1", "data_2", "data_3"}
var exeDir string = getExeDir()

func readAndSeparateFile(fileInfo fs.FileInfo, discordCacheFolder string) {
	if fileInfo.IsDir() {
		return
	}

	for _, item := range knownUnreadableFiles {
		if item == fileInfo.Name() {
			return
		}
	}

	filePath := filepath.Join(discordCacheFolder, fileInfo.Name())

	buffer, err := getFileBuffer(filePath, fileInfo)
	if err != nil {
		fmt.Println("Could not read buffer for file: ", fileInfo.Name())
		return
	}

	fileData, err := getNewFileData(buffer)

	if err != nil && err.Error() == "unknown filetype" {
		return
	}

	if err != nil {
		fmt.Println("Encountered error while trying to determine file type for file: ", fileInfo.Name())
		return
	}

	saveDir := getOrCreateSaveDir(exeDir, fileData.fileExtension)

	buffer = buffer[fileData.startingByte:]
	fileExists, newFilePath := getNewFilePath(buffer, saveDir, fileInfo.Name(), fileData.fileExtension)

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

	if bytes.Equal(fileData, buffer) {
		return true
	} else {
		return false
	}
}

func getNewFilePath(buffer []byte, saveDir string, fileName string, fileExtension string) (bool, string) {
	filePath := filepath.Join(saveDir, fmt.Sprintf("%s.%s", fileName, fileExtension))

	if fileNameExists(filePath) && isSameFile(buffer, filePath) {
		return true, ""
	}

	i := 0
	for fileNameExists(filePath) {
		i++
		filePath = fmt.Sprintf("%d_%s", i, filePath)
	}

	return false, filePath
}

func getFileBuffer(filePath string, fileInfo fs.FileInfo) ([]byte, error) {
	file, err := os.Open(filePath)

	if err != nil {
		fmt.Println("Error opening file: ", filePath, " due to err: ", err)
		return nil, errors.New("")
	}
	defer file.Close()

	buffer := make([]byte, fileInfo.Size())
	_, err = file.Read(buffer)
	if err != nil {
		fmt.Println("Error reading file: ", filePath, " due to err: ", err)
		return nil, errors.New("")
	}

	return buffer, nil
}

func getExeDir() string {
	exePath, err := os.Executable()
	if err != nil {
		fmt.Println("Error retrieving executable path: ", err)
		os.Exit(1)
	}

	return filepath.Dir(exePath)
}

func getOrCreateSaveDir(exeDir string, fileExtension string) string {
	saveDir := filepath.Join(exeDir, fileExtension)
	_, err := os.Stat(saveDir)
	if os.IsNotExist(err) {
		os.Mkdir(saveDir, 0755)
	} else if err != nil {
		fmt.Println("Error:", err)
	}

	return saveDir
}

func getDiscordCacheFolderBasedOnOS() string {
	operatingSystem := runtime.GOOS
	if operatingSystem == "windows" {
		userConfigDir, err := os.UserConfigDir()
		if err != nil {
			fmt.Println("Something went wrong when grabbing the discord cache directory:", err)
			os.Exit(1)
			return ""
		}
		return filepath.Join(userConfigDir, "discord/Cache/Cache_Data")
	} else if operatingSystem == "darwin" {
		userHomeDir, err := os.UserHomeDir()
		if err != nil {
			fmt.Println("Something went wrong when grabbing the discord cache directory:", err)
			os.Exit(1)
			return ""
		}
		return filepath.Join(userHomeDir, "Library/Application Support/discord/Cache/Cache_Data")
	} else if operatingSystem == "linux" {
		userHomeDir, err := os.UserHomeDir()
		if err != nil {
			fmt.Println("Something went wrong when grabbing the discord cache directory:", err)
			os.Exit(1)
			return ""
		}
		return filepath.Join(userHomeDir, ".config/discord/Cache/Cache_Data")
	} else {
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

func getFileExtension(buffer []byte) (cachedFileData, error) {
	fileInfo, _ := filetype.Match(buffer)
	if fileInfo == filetype.Unknown {
		return cachedFileData{}, errors.New("unknown filetype")
	}

	return cachedFileData{fileInfo.Extension, 0}, nil
}

func getFileExtensionLinux(buffer []byte) (cachedFileData, error) {
	for i := 0; i < 70; i++ {
		if i >= len(buffer) {
			break
		}

		fileInfo, _ := filetype.Match(buffer[i:])

		if fileInfo == filetype.Unknown {
			continue
		}

		return cachedFileData{fileInfo.Extension, 0}, nil
	}

	return cachedFileData{}, errors.New("unknown filetype")
}
