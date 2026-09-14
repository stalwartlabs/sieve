# Redirect, notify and copy
#
# Sends a copy of invoices to accounting, notifies Jane by email about
# urgent messages, and rewrites the subject with "editheader" before the
# message is kept.

require ["copy", "enotify", "variables", "editheader", "redirect-dsn",
         "fileinto", "fcc", "mailbox", "envelope"];

if header :contains "Subject" "invoice" {
    redirect :copy :notify "failure,delay" :ret "hdrs" "accounting@example.org";
}

set "from" "";
if anyof (header :is "Importance" "high", header :contains "Subject" "urgent") {
    if header :matches "From" "*" {
        set "from" "${1}";
    }
    notify :importance "1"
           :message "Urgent mail from ${from}"
           "mailto:jane.mobile@example.org?subject=Urgent%20mail";
}

if header :matches "Subject" "*" {
    deleteheader "Subject";
    addheader "Subject" "[Finance] ${1}";
}

fileinto :copy :create "Finance";
