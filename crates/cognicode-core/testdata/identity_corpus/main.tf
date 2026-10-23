resource "aws_s3_bucket" "app_bucket" {
  bucket = "app-data"
}

resource "aws_s3_bucket_policy" "app_policy" {
  bucket = aws_s3_bucket.app_bucket.id
}
