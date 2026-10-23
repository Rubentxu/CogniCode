resource "aws_s3_bucket" "broken_bucket" {
  bucket = "app-data"
}

resource "aws_s3_bucket_policy" "broken_policy" {
  bucket = aws_s3_bucket.broken_bucket.id
}
}
