require ["reject", "ereject"];

if address "from" "someone@example.com" {
    ereject "I no longer accept mail from this address";
}

if size :over 100K {
    reject text:
Your message is too big.  If you want to send me a big attachment,
put it on a public web site and send me a URL.
.
    ;
}
if header :contains "from" "coyote@desert.example.org" {
    reject text:
I am not taking mail from you, and I don't
want your birdseed, either!
.
    ;
}

