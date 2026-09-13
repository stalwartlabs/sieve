# Quarantine spam and viruses
#
# The spam and virus scores come from Settings (Spam and virus). Try
# changing them and running the script again.

require ["spamtest", "spamtestplus", "virustest", "relational",
         "comparator-i;ascii-numeric", "fileinto", "mailbox", "special-use",
         "imap4flags", "reject", "editheader", "variables"];

if virustest :value "eq" :comparator "i;ascii-numeric" "5" {
    reject "Message rejected: a virus was detected.";
    stop;
}

if spamtest :value "ge" :comparator "i;ascii-numeric" "8" {
    discard;
    stop;
}

if spamtest :percent :value "ge" :comparator "i;ascii-numeric" "50" {
    if spamtest :percent :matches "*" {
        set "score" "${1}";
    }
    addheader "X-Spam-Score" "${score}%";
    addflag "$Junk";
    fileinto :specialuse "\\Junk" :create "Junk";
}
