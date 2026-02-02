package main

import (
	"fmt"
	"os"
	"runtime"
	"sync"
)

var version = "dev"

func main() {
	fmt.Printf("Discord Cache Recovery Tool %s\n", version)
	fmt.Println("Locating discord cache folder")
	discordCacheFolder := getDiscordCacheFolderBasedOnOS()

	fmt.Println("Reading cache folder for all saved files.")
	dirEntries, err := os.ReadDir(discordCacheFolder)
	if err != nil {
		fmt.Println("Error reading folder:", err)
		os.Exit(1)
	}

	// Worker pool: we process files across multiple goroutines to speed up the recovery.
	numWorkers := runtime.NumCPU()
	jobs := make(chan os.DirEntry, len(dirEntries))
	var wg sync.WaitGroup

	// Each worker will read and separate files concurrently.
	fmt.Println("Recovering found files.")
	for i := 0; i < numWorkers; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for dirEntry := range jobs {
				readAndSeparateFile(dirEntry, discordCacheFolder)
			}
		}()
	}

	// After we have declared the workers, we send the jobs.
	for _, dirEntry := range dirEntries {
		jobs <- dirEntry
	}
	close(jobs)

	// Wait for all workers to finish.
	wg.Wait()

	fmt.Println("Done!")
	fmt.Println("You can close the window or press enter to exit...")
	fmt.Scanln()
}
