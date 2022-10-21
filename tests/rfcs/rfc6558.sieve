require ["convert", "fileinto"];
convert "image/tiff" "image/jpeg" ["pix-x=320","pix-y=240"];
if (convert "image/tiff" "image/jpeg" ["pix-x=320","pix-y=240"])
{
    fileinto "INBOX.pics";
}
