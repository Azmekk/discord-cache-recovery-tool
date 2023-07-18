package main

import (
	"bytes"
	"fmt"
	"io/fs"
	"io/ioutil"
	"mime"
	"os"
	"path/filepath"
	"runtime"

	"github.com/gabriel-vasile/mimetype"
	"github.com/h2non/filetype"
)

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
	exeDir := getExeDir()
	buffer := getFileBuffer(filePath, fileInfo)
	fileType, unknownFileTypeErr, firstFileByte := getFileMimeType(buffer)
	buffer = buffer[firstFileByte:]
	if unknownFileTypeErr != nil {
		return
	} else if fileType == "application/octet-stream" {
		return
	}
	fileExtensions := getFileExtensions(fileType, filePath)

	if len(fileExtensions) == 0 {
		return
	}
	fileExtension := fileExtensions[0]

	saveDir := getSaveDirAndCreateIfNotExists(exeDir, fileExtension)
	sameFileAlreadyExists, newFilePath := getNewFilePath(buffer, saveDir, fileInfo.Name(), fileExtension, 0)

	if sameFileAlreadyExists {
		return
	}

	err := ioutil.WriteFile(newFilePath, buffer, 0644)
	if err != nil {
		fmt.Println(fmt.Sprintf("Error writing file %s:%s", filePath, err))
		return
	}
}

func doesFileNameExist(filePath string) bool {
	_, err := os.Stat(filePath)
	if os.IsNotExist(err) {
		return false
	}
	return true
}

func isSameFile(buffer []byte, existingFilePath string) bool {
	fileData, err := ioutil.ReadFile(existingFilePath)
	if err != nil {
		fmt.Println(fmt.Sprintf("Error reading file to compare with buffer %s: %s", existingFilePath, err))
		os.Exit(1)
	}

	if bytes.Equal(fileData, buffer) {
		return true
	} else {
		return false
	}
}

func getNewFilePath(buffer []byte, saveDir string, fileName string, fileExtension string, depth int) (bool, string) {
	depth++
	filePath := filepath.Join(saveDir, fileName+fileExtension)

	fileNameAlreadyExists := doesFileNameExist(filePath)

	if fileNameAlreadyExists && isSameFile(buffer, filePath) {
		return true, ""
	} else if fileNameAlreadyExists {
		fileName = fmt.Sprintf("%s_%d", fileName, depth)
		return getNewFilePath(buffer, saveDir, fileName, fileExtension, depth)
	}

	return false, filePath
}

func detectOS() string {
	os := runtime.GOOS
	return os
}

func getFileBuffer(filePath string, fileInfo fs.FileInfo) []byte {
	file, err := os.Open(filePath)
	if err != nil {
		fmt.Println(fmt.Sprintf("Error opening file %s: %s", filePath, err))
		os.Exit(1)
	}
	defer file.Close()

	buffer := make([]byte, fileInfo.Size())
	_, err = file.Read(buffer)
	if err != nil {
		fmt.Println(fmt.Sprintf("Error reading file %s: %s", filePath, err))
		os.Exit(1)
	}

	return buffer
}

func getExeDir() string {
	exePath, err := os.Executable()
	if err != nil {
		fmt.Println("Error retrieving executable path:", err)
		os.Exit(1)
	}

	return filepath.Dir(exePath)
}

func getFileExtensions(fileType string, filePath string) []string {
	fileExtensions, err := mime.ExtensionsByType(fileType)
	if err != nil {
		fmt.Println(fmt.Sprintf("Error getting extension for mime type %s for file %s: %s", fileType, filePath, err))
		os.Exit(1)
	}

	return fileExtensions
}

func getSaveDirAndCreateIfNotExists(exeDir string, fileExtension string) string {
	saveDir := filepath.Join(exeDir, fileExtension[1:])
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

func getFileMimeType(buffer []byte) (string, error, int) {
	if runtime.GOOS == "windows" {
		return mimetype.Detect(buffer).String(), nil, 0
	} else {
		return detectUnixFileMIMEType(buffer)
	}
}

func detectUnixFileMIMEType(buffer []byte) (string, error, int) {
	for i := 0; i < 400; i++ {
		if i >= len(buffer) {
			break
		}

		kind, _ := filetype.Match(buffer[i:])

		if kind != filetype.Unknown {
			return kind.MIME.Value, nil, i
		}
	}

	return "", fmt.Errorf("Unknown filetype"), 0
}
