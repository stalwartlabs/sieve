require ["include", "variables"];

global ["priority", "project"];

set "project" "general";
if header :matches "Subject" "[*]*" {
    set :lower "project" "${1}";
}

set "priority" "normal";
if header :contains "Subject" ["blocker", "outage"] {
    set "priority" "high";
}
