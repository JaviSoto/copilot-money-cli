package cli

import "testing"

func TestOutputFormatFlag(t *testing.T) {
	var f OutputFormat = OutputFormatTable
	flag := &outputFormatFlag{value: &f}
	if err := flag.Set("json"); err != nil {
		t.Fatalf("Set: %v", err)
	}
	if f != OutputFormatJSON {
		t.Fatalf("expected %q, got %q", OutputFormatJSON, f)
	}
}
