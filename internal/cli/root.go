package cli

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"

	"github.com/jedib0t/go-pretty/v6/table"
	"github.com/spf13/cobra"
)

type OutputFormat string

const (
	OutputFormatJSON  OutputFormat = "json"
	OutputFormatTable OutputFormat = "table"
)

type rootFlags struct {
	output OutputFormat
}

func NewRootCmd() *cobra.Command {
	flags := &rootFlags{
		output: OutputFormatTable,
	}

	cmd := &cobra.Command{
		Use:           "copilot",
		Short:         "CLI for Copilot Money (unofficial)",
		SilenceUsage:  true,
		SilenceErrors: true,
		PersistentPreRunE: func(cmd *cobra.Command, args []string) error {
			if flags.output != OutputFormatJSON && flags.output != OutputFormatTable {
				return fmt.Errorf("invalid --output %q (expected: json|table)", flags.output)
			}
			return nil
		},
	}

	cmd.PersistentFlags().StringVar(&cmd.Version, "version", "", "Print version (alias: `version` command)")
	_ = cmd.PersistentFlags().MarkHidden("version")
	cmd.PersistentFlags().Var(&outputFormatFlag{value: &flags.output}, "output", "Output format: json|table")

	cmd.AddCommand(newVersionCmd())
	cmd.AddCommand(newHelloCmd(flags))

	return cmd
}

type outputFormatFlag struct {
	value *OutputFormat
}

func (f *outputFormatFlag) String() string {
	if f.value == nil {
		return ""
	}
	return string(*f.value)
}

func (f *outputFormatFlag) Set(s string) error {
	*f.value = OutputFormat(s)
	return nil
}

func (f *outputFormatFlag) Type() string {
	return "output"
}

func newVersionCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "version",
		Short: "Print version info",
		RunE: func(cmd *cobra.Command, args []string) error {
			fmt.Println("copilot-money-api (scaffold)")
			return nil
		},
	}
}

func newHelloCmd(flags *rootFlags) *cobra.Command {
	type row struct {
		Key   string `json:"key"`
		Value string `json:"value"`
	}

	return &cobra.Command{
		Use:    "hello",
		Short:  "Sanity check output modes",
		Hidden: true,
		RunE: func(cmd *cobra.Command, args []string) error {
			rows := []row{
				{Key: "status", Value: "ok"},
				{Key: "next", Value: "capture GraphQL endpoints"},
			}

			switch flags.output {
			case OutputFormatJSON:
				enc := json.NewEncoder(os.Stdout)
				enc.SetIndent("", "  ")
				return enc.Encode(rows)
			case OutputFormatTable:
				tw := table.NewWriter()
				tw.SetOutputMirror(os.Stdout)
				tw.AppendHeader(table.Row{"Key", "Value"})
				for _, r := range rows {
					tw.AppendRow(table.Row{r.Key, r.Value})
				}
				tw.Render()
				return nil
			default:
				return errors.New("unreachable output format")
			}
		},
	}
}
