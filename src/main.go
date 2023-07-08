package main

import (
	"fmt"
	"io/ioutil"
	"os"
	"runtime"
)

var knownUnreadableFiles = []string{"index", "data_0", "data_1", "data_2", "data_3"}

func main() {
	fmt.Println("Locating discord cache folder")
	discordCacheFolder := func() string {
		operatingSystem := runtime.GOOS
		if operatingSystem == "windows" {
			return fmt.Sprintf("%s\\discord\\Cache\\Cache_Data", os.Getenv("APPDATA"))
		} else if operatingSystem == "darwin" {
			return "~/Library/Application Support/discord"
		} else if operatingSystem == "linux" {
			return "~/.config/discord/Cache/Cache_Data"
		} else {
			fmt.Println("Unrecognized OS")
			os.Exit(1)
			return ""
		}
	}()

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
