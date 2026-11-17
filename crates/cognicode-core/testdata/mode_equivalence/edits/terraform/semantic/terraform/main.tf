resource "aws_s3_bucket" "logs" {
  bucket = var.bucket_name
}

resource "aws_s3_bucket_policy" "logs" {
  bucket = aws_s3_bucket.logs.id
  policy = "{}"
}

module "network" {
  source = "./modules/network"
  cidr   = var.cidr
}

module "dns" {
  source = "./modules/dns"
}
