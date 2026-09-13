require ["include", "variables", "regex"];

global ["priority", "project"];

set "project" "general";
if header :regex "Subject" "^\\[([A-Za-z0-9-]+)\\]" {
    set :lower "project" "${1}";
}

set "priority" "normal";
if header :contains "Subject" ["blocker", "outage"] {
    set "priority" "high";
}
