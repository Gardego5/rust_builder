data "archive_file" "image_lambda" {
  type        = "zip"
  source_dir  = "../bin/image_lambda"
  output_path = "image_lambda.zip"
}

resource "aws_lambda_function" "image_lambda" {
  filename         = data.archive_file.image_lambda.output_path
  function_name    = "${var.app_prefix}-image-lambda"
  role             = aws_iam_role.image_lambda.arn
  handler          = "bootstrap"
  source_code_hash = data.archive_file.image_lambda.output_base64sha256
  architectures    = ["arm64"]
  runtime          = "provided.al2"
  timeout          = 30
  memory_size      = 1024
  environment {
    variables = {
      "BUCKET_NAME" = resource.aws_s3_bucket.image_bucket.id
      "REGION"      = var.region
    }
  }
}

resource "aws_cloudwatch_log_group" "image_lambda" {
  name              = "/aws/lambda/${aws_lambda_function.image_lambda.function_name}"
  retention_in_days = 1
}

resource "aws_lambda_function_url" "image_lambda" {
  function_name      = aws_lambda_function.image_lambda.function_name
  authorization_type = "NONE"
}

data "aws_iam_policy_document" "image_lambda_assume_role" {
  statement {
    effect  = "Allow"
    actions = ["sts:AssumeRole"]
    principals {
      type        = "Service"
      identifiers = ["lambda.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "image_lambda" {
  name               = "${var.app_prefix}-image-lambda"
  assume_role_policy = data.aws_iam_policy_document.image_lambda_assume_role.json
}

data "aws_iam_policy_document" "image_lambda" {
  statement {
    effect    = "Allow"
    actions   = ["logs:CreateLogGroup", "logs:CreateLogStream", "logs:PutLogEvents"]
    resources = ["arn:aws:logs:*:*:*"]
  }
}

resource "aws_iam_role_policy" "image_lambda" {
  role   = aws_iam_role.image_lambda.name
  policy = data.aws_iam_policy_document.image_lambda.json
}

resource "aws_iam_role_policy_attachment" "image_lambda_read_image_bucket" {
  role       = aws_iam_role.image_lambda.name
  policy_arn = aws_iam_policy.image_bucket_read.arn
}
