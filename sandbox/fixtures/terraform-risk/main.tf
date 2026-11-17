provider "aws" {
  region     = "eu-west-1"
  access_key = "AKIAIOSFODNN7EXAMPLE"
  secret_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
}

module "storage" {
  source = "./modules/storage"

  acl       = var.bucket_acl
  encrypted = var.encrypt_volumes
}

module "network" {
  source = "./modules/network"

  admin_cidr = var.ssh_cidr
}

module "shared" {
  source  = "terraform-aws-modules/s3-bucket/aws"
  version = "4.1.0"

  bucket = "shared-${var.environment}"
}

resource "aws_s3_bucket" "logs" {
  bucket = "logs-${var.environment}"
  acl    = "public-read"
}

resource "aws_db_instance" "main" {
  identifier          = "main-${var.environment}"
  engine              = "postgres"
  instance_class      = "db.t3.micro"
  username            = "admin"
  password            = "Sup3rS3cretPassw0rd!"
  publicly_accessible = true
  storage_encrypted   = false
}

resource "aws_iam_policy" "ops" {
  name = "ops"
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = var.admin_action
      Resource = "*"
    }]
  })
}

resource "aws_efs_file_system" "shared" {
  encrypted = var.audit_enabled
}
