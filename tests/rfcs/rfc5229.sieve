require ["encoded-character", "variables", "fileinto"];

set "dollar" "$";
set "text" "regarding ${dollar}{beep}";

set "name" "Ethelbert";
if header :contains "Subject" "dear${hex:20 24 7b 4e}ame}" {
    # the test string is "dear Ethelbert"
}

if header :matches "List-ID" "*<*@*" {
    fileinto "INBOX.lists.${2}"; stop;
}

# Imagine the header
# Subject: [acme-users] [fwd] version 1.0 is out
if header :matches "Subject" "[*] *" {
    # ${1} will hold "acme-users",
    # ${2} will hold "[fwd] version 1.0 is out"
    fileinto "INBOX.lists.${1}"; stop;
}

# Imagine the header
# To: coyote@ACME.Example.COM
if address :matches ["To", "Cc"] ["coyote@**.com",
        "wile@**.com"] {
    # ${0} is the matching address
    # ${1} is always the empty string
    # ${2} is part of the domain name ("ACME.Example")
    fileinto "INBOX.business.${2}"; stop;
} else {
    # Control wouldn't reach this block if any match was
    # successful, so no match variables are set at this
    # point.
}

if anyof (true, address :domain :matches "To" "*.com") {
    # The second test is never evaluated, so there are
    # still no match variables set.
    stop;
}

set "honorific"  "Mr";
set "first_name" "Wile";
set "last_name"  "Coyote";
set "vacation" text:
Dear ${HONORIFIC} ${last_name},
I'm out, please leave a message after the meep.
.
;


set "a" "juMBlEd lETteRS";             #=> "juMBlEd lETteRS"
set :length "b" "${a}";                #=> "15"
set :lower "b" "${a}";                 #=> "jumbled letters"
set :upperfirst "b" "${a}";            #=> "JuMBlEd lETteRS"
set :upperfirst :lower "b" "${a}";     #=> "Jumbled letters"
set :quotewildcard "b" "Rock*";        #=> "Rock\*"

set "state" "${state} pending";
if string :matches " ${state} " "* pending *" {
    # the above test always succeeds
}

