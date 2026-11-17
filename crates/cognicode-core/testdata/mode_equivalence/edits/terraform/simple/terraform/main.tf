# A comment-only and attribute-only edit.
resource "aws_s3_bucket" "logs" {
  bucket = var.bucket_name
  tags = {
    Team = "platform"
  }
}

resource "aws_s3_bucket_versioning" "logs" {
  bucket = aws_s3_bucket.logs.id

  versioning_configuration {
    status = "Enabled"
  }
}

module "network" {
  source = "./modules/network"
  cidr   = var.cidr
}
