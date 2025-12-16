package main

import (
	"os"

	"github.com/javisoto/copilot-money-api/internal/cli"
)

func main() {
	if err := cli.NewRootCmd().Execute(); err != nil {
		os.Exit(1)
	}
}
