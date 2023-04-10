resource "aws_s3_bucket" "image_bucket" {
  bucket = "${var.app_prefix}-image-bucket"
}

data "aws_iam_policy_document" "image_bucket_read" {
  statement {
    actions = [
      "s3:GetObject",
      "s3:ListBucket"
    ]

    resources = [
      aws_s3_bucket.image_bucket.arn,
      "${aws_s3_bucket.image_bucket.arn}/*"
    ]
  }
}

resource "aws_iam_policy" "image_bucket_read" {
  policy = data.aws_iam_policy_document.image_bucket_read.json
  name   = "${var.app_prefix}-image-bucket-read"
}
