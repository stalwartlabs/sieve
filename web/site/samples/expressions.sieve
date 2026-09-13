# Stalwart expressions and loops
#
# "vnd.stalwart.expressions" adds "let" and "eval" with arithmetic,
# string functions and arrays. "vnd.stalwart.while" adds loops. Both are
# Stalwart extensions to Sieve.

require ["variables", "editheader", "fileinto", "mailbox", "imap4flags",
         "vnd.stalwart.expressions", "vnd.stalwart.while"];

let "recipients" "header.to:cc[*].addr[*]";
let "total" "count(recipients)";
let "shouting" "is_uppercase(header.subject)";

if eval "total > 3" {
    addflag "$ManyRecipients";
}

let "i" "0";
let "domains" "''";
while "i < total" {
    let "domain" "email_part(recipients[i], 'domain')";
    let "domains" "domains + domain + ' '";
    let "i" "i + 1";
}
addheader "X-Recipient-Domains" "${domains}";

if eval "shouting" {
    addflag "$Shouting";
    let "subject" "to_lowercase(header.subject)";
    deleteheader "Subject";
    addheader "Subject" "${subject}";
}

if eval "contains_ignore_case(header.subject, 'meeting')" {
    fileinto :create "Calendar";
}
