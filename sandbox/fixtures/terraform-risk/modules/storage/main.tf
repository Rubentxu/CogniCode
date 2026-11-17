resource "aws_s3_bucket" "data" {
  bucket = "data-bucket"
  acl    = var.acl
}

resource "aws_ebs_volume" "scratch" {
  availability_zone = "eu-west-1a"
  size              = 20
  encrypted         = var.encrypted
}

output "bucket_acl" {
  value = aws_s3_bucket.data.acl
}
