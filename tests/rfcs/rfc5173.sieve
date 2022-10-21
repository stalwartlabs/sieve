require ["body", "fileinto"];

if body :raw :contains "MAKE MONEY FAST" {
        discard;
}

# Save any message with any text MIME part that contains the
# words "missile" or "coordinates" in the "secrets" folder.

if body :content "text" :contains ["missile", "coordinates"] {
        fileinto "secrets";
}

# Save any message with an audio/mp3 MIME part in
# the "jukebox" folder.

if body :content "audio/mp3" :contains "" {
        fileinto "jukebox";
}

# Save messages mentioning the project schedule in the
# project/schedule folder.
if body :text :contains "project schedule" {
        fileinto "project/schedule";
}
