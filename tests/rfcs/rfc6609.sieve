require ["include", "reject", "fileinto", "variables", "relational", "vacation"];

include :personal "always_allow";
include :global "spam_tests";
include :personal "spam_tests";
include :personal "mailing_lists";


if address :is "from" "boss@example.com"
{
    keep;
}
elsif address :is "from" "ceo@example.com"
{
    keep;
}

if header :contains "Subject" "XXXX"
{
    reject "Subject XXXX is unacceptable.";
}
elsif address :is "from" "money@example.com"
{
    reject "Mail from this sender is unwelcome.";
}

if header :is "List-ID" "sieve.ietf.org"
{
    fileinto "lists.sieve";
}
elsif header :is "List-ID" "ietf-imapext.imc.org"
{
    fileinto "lists.imapext";
}

if anyof (header :contains "Subject" "$$",
        header :contains "Subject" "Make money")
{
    reject "No thank you.";
}

global "test";
global "test_mailbox";

set "test" "$$";
include "subject_tests";

set "test" "Make money";
include "subject_tests";

if string :count "eq" "${test_mailbox}" "1"
{
    fileinto "${test_mailbox}";
    stop;
}

if header :contains "Subject" "${test}"
{
    set "test_mailbox" "spam-${test}";
}

set "global.i_am_on_vacation" "1";

global "i_am_on_vacation";

set "global.i_am_on_vacation" "1";

if string :is "${i_am_on_vacation}" "1"
{
    vacation "It's true, I am on vacation.";
}

if string :is "${global.i_am_on_vacation}" "1"
{
    vacation "It's true, I am on vacation.";
}

set "envelope.from" :replace "." "-" "t.e.s.t@domain.com";
