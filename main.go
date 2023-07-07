package main

import (
	"bytes"
	"fmt"
	"io/fs"
	"io/ioutil"
	"mime"
	"net/http"
	"os"
	"path/filepath"
)

func main() {

	discordCacheFolder := fmt.Sprintf("%s\\discord\\Cache\\Cache_Data", os.Getenv("APPDATA"))

	fmt.Println("Reading cache folder for all saved files.")
	cachedFiles, err := ioutil.ReadDir(discordCacheFolder)
	if err != nil {
		fmt.Println("Error reading folder:", err)
		os.Exit(1)
	}

	fmt.Println("Iterating through files to determine type and place them in the respective folder.")
	for _, file := range cachedFiles {
		readAndSeparateFile(file, discordCacheFolder)
	}
}

func readAndSeparateFile(fileInfo fs.FileInfo, discordCacheFolder string) {
	if fileInfo.IsDir() {
		return
	}

	filePath := filepath.Join(discordCacheFolder, fileInfo.Name())

	exePath, err := os.Executable()
	if err != nil {
		fmt.Println("Error retrieving executable path:", err)
		os.Exit(1)
	}

	exeDir := filepath.Dir(exePath)

	file, err := os.Open(filePath)
	if err != nil {
		fmt.Println(fmt.Sprintf("Error opening file %s: %s", filePath, err))
		os.Exit(1)
	}
	defer file.Close()

	buffer := make([]byte, fileInfo.Size()) // Buffer to read the file content
	_, err = file.Read(buffer)
	if err != nil {
		fmt.Println(fmt.Sprintf("Error reading file %s: %s", filePath, err))
		os.Exit(1)
	}

	fileType := http.DetectContentType(buffer)
	fileExtensions, err := mime.ExtensionsByType(fileType)
	if err != nil {
		fmt.Println(fmt.Sprintf("Error getting extension for mime type %s for file %s: %s", fileType, filePath, err))
		os.Exit(1)
	}

	if len(fileExtensions) == 0 {
		fmt.Println(fmt.Sprintf("Skipping file %s due to it being of fileType %s which has no known extensions", filePath, fileType))
		return
	}

	fileExtension := fileExtensions[0]

	saveDir := fmt.Sprintf("%s\\%s", exeDir, fileExtension[1:])
	_, err = os.Stat(saveDir)
	if os.IsNotExist(err) {
		os.Mkdir(saveDir, 0755)
	} else if err != nil {
		fmt.Println("Error:", err)
	}

	sameFileAlreadyExists, newFilePath := GetNewFilePathIfFileDoesNotExist(buffer, saveDir, fileInfo.Name(), fileExtension, 0)

	if sameFileAlreadyExists {
		return
	}

	newFile, err := os.Create(newFilePath)
	if err != nil {
		fmt.Println("Error creating new file:", err)
		return
	}

	defer file.Close()
	_, err = newFile.Write(buffer)
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

func GetNewFilePathIfFileDoesNotExist(buffer []byte, saveDir string, fileName string, fileExtension string, depth int) (bool, string) {
	depth++
	filePath := fmt.Sprintf("%s\\%s%s", saveDir, fileName, fileExtension)

	fileNameAlreadyExists := doesFileNameExist(filePath)

	if fileNameAlreadyExists && isSameFile(buffer, filePath) {
		return true, ""
	} else if fileNameAlreadyExists {
		fileName = fmt.Sprintf("%s_%d", fileName, depth)
		return GetNewFilePathIfFileDoesNotExist(buffer, saveDir, fileName, fileExtension, depth)
	}

	return false, filePath
}
