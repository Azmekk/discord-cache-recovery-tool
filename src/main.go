package main

import (
	"fmt"
	"io/ioutil"
	"os"
)

var knownUnreadableFiles = []string{"index", "data_0", "data_1", "data_2", "data_3"}

func main() {
	fmt.Println("Locating discord cache folder")
	discordCacheFolder := getDiscordCacheFolderBasedOnOS()

	fmt.Println("Reading cache folder for all saved files.")
	cachedFiles, err := ioutil.ReadDir(discordCacheFolder)
	if err != nil {
		fmt.Println("Error reading folder:", err)
		os.Exit(1)
	}

	fmt.Println("Recovering found files.")
	for _, file := range cachedFiles {
		readAndSeparateFile(file, discordCacheFolder)
	}

	fmt.Println("Done!")
}
