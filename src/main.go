package main

import (
	"fmt"
	"os"
)

func main() {
	fmt.Println("Locating discord cache folder")
	discordCacheFolder := getDiscordCacheFolderBasedOnOS()

	fmt.Println("Reading cache folder for all saved files.")
	dirEntries, err := os.ReadDir(discordCacheFolder)
	if err != nil {
		fmt.Println("Error reading folder:", err)
		os.Exit(1)
	}

	fmt.Println("Recovering found files.")
	for _, dirEntry := range dirEntries {
		fileInfo, err := dirEntry.Info()
		if err != nil {
			fmt.Println("Error reading file ", dirEntry.Name())
			continue
		}
		readAndSeparateFile(fileInfo, discordCacheFolder)
	}

	fmt.Println("Done!")
	fmt.Println("You can close the window or press enter to exit...")
	fmt.Scanln()
}
