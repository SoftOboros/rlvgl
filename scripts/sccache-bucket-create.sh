aws s3api create-bucket --bucket $1 --region us-east-1
aws s3api put-bucket-lifecycle-configuration \
  --bucket $1 \
  --lifecycle-configuration file://scripts/sccache-bucket-lifecycle.json
