variable "admin_cidr" {
  type = string
}

locals {
  admin_sources = [var.admin_cidr]
}

resource "aws_security_group" "admin" {
  name = "admin"

  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = local.admin_sources
  }
}

resource "aws_security_group" "open" {
  name = "open"

  ingress {
    from_port   = 3389
    to_port     = 3389
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  ingress {
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }
}
