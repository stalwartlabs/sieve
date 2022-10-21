require ["vacation", "vacation-seconds"];
if header :contains "subject" "cyrus" {
    vacation "I'm out -- send mail to cyrus-bugs";
} else {
    vacation "I'm out -- call me at +1 304 555 0123";
}

if header :matches "subject" "*" {
    vacation :subject "Automatic response to: ${1}"
            "I'm away -- send mail to foo in my absence";
}

if header :contains "subject" "lunch" {
    vacation :handle "ran-away" "I'm out and can't meet for lunch";
} else {
    vacation :handle "ran-away" "I'm out";
}

vacation :mime text:
Content-Type: multipart/alternative; boundary=foo

--foo

I'm at the beach relaxing.  Mmmm, surf...

--foo
Content-Type: text/html; charset=us-ascii

<!DOCTYPE HTML PUBLIC "-//W3C//DTD HTML 4.0//EN"
"http://www.w3.org/TR/REC-html40/strict.dtd">
<HTML><HEAD><TITLE>How to relax</TITLE>
<BASE HREF="http://home.example.com/pictures/"></HEAD>
<BODY><P>I'm at the <A HREF="beach.gif">beach</A> relaxing.
Mmmm, <A HREF="ocean.gif">surf</A>...
</BODY></HTML>

--foo--
.
;

vacation :days 23 :addresses ["tjs@example.edu",
                                "ts4z@landru.example.edu"]
"I'm away until October 19.
If it's an emergency, call 911, I guess." ;

if header :contains "from" "boss@example.edu" {
    redirect "pleeb@isp.example.org";
} else {
    vacation "Sorry, I'm away, I'll read your
message when I get around to it.";
}

if header :contains ["accept-language", "content-language"] "en"
{
    vacation "I am away this week.";
} else {
    vacation "Estoy ausente esta semana.";
}

if address :matches "from" "*@ourdivision.example.com"
{
    vacation :subject "Gone fishing"
            "Having lots of fun! Back in a day or two!";
} else {
    vacation :subject "Je suis parti cette semaine"
            "Je lirai votre message quand je retourne.";
}

vacation :addresses ["tjs@example.edu", "ts4z@landru.example.edu"]
        :seconds 1800
        "I am in a meeting, and do not have access to email.";

vacation :handle "auto-resp" :seconds 0
    "Your request has been received.  A service
    representative will contact you as soon as
    possible, usually within one business day.";
