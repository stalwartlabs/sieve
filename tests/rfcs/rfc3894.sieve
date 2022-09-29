require ["copy", "fileinto"];
fileinto :copy "incoming";
redirect :copy "test";
