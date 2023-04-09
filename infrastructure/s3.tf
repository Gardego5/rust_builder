resource "aws_s3_bucket" "image_bucket" {
  bucket_prefix = "${var.app_prefix}-image-bucket"
}
