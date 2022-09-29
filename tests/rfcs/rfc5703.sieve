require ["mime", "foreverypart", "fileinto", "replace", "enclose"];

if header :mime :type "Content-Type" "image"
{
    fileinto "INBOX.images";
}

if header :mime :anychild :contenttype
            "Content-Type" "text/html"
{
    fileinto "INBOX.html";
}

foreverypart
{
    if allof (
        header :mime :param "filename" :contains
        "Content-Disposition" "important",
        header :mime :subtype "Content-Type" "pdf",
        size :over 100K)
    {
        fileinto "INBOX.important";
        break;
    }
}

if address :mime :is :all "content-from" "tim@example.com"
{
    fileinto "INBOX.part-from-tim";
}

if exists :mime :anychild "content-md5"
{
    fileinto "INBOX.md5";
}


foreverypart
{
    if anyof (
        header :mime :contenttype :is
            "Content-Type" "application/exe",
        header :mime :param "filename"
            :matches ["Content-Type", "Content-Disposition"] "*.com" )
    {
    replace "Executable attachment removed by user filter";
    }
}

foreverypart :name "main_loop"
{
    if header :mime :param "filename"
    :matches ["Content-Type", "Content-Disposition"]
        ["*.com", "*.exe", "*.vbs", "*.scr",
        "*.pif", "*.hta", "*.bat", "*.zip" ]
    {
    # these attachment types are executable
    enclose :subject "Warning" text:
Warning
.
    ;
        break :name "main_loop";
    }
}


if header :contains "from" "boss@example.org"
{
    # :matches is used to get the value of the Subject header
    if header :matches "Subject" "*"
    {
    set "subject" "${1}";
    }

    # extract the first 100 characters of the first text/* part
    foreverypart
    {
    if header :mime :type :is "Content-Type" "text"
    {
        extracttext :first 100 "msgcontent";
        break;
    }
    }

    # if it's not a 'for your information' message
    if not header :contains "subject" "FYI:"
    {
    # do something using ${subject} and ${msgcontent}
    # such as sending a notification using a
    # notification extension
    }
}