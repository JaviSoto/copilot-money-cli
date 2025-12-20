# vhs demo for copilot-money-cli

Set Shell "bash"
Set FontSize 18
Set Width 1200
Set Height 720
Set TypingSpeed 70ms
Set Theme "Catppuccin Mocha"

Type "cd $(pwd)"
Enter

Type "export COPILOT_FIXTURES_DIR=tests/fixtures/graphql"
Enter
Type "export HOME=$(mktemp -d)"
Enter

Type "./target/release/copilot auth status"
Enter
Sleep 800ms

Type "./target/release/copilot transactions list --unreviewed --fields date,name,amount,category"
Enter
Sleep 900ms

Type "./target/release/copilot transactions show txn_2"
Enter
Sleep 900ms

Type "./target/release/copilot categories list"
Enter
Sleep 900ms

Type "./target/release/copilot recurrings list"
Enter
Sleep 900ms

Type "./target/release/copilot --output json transactions list --limit 2"
Enter
Sleep 900ms
