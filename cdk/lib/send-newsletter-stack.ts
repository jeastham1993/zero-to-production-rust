import { RustFunction } from '@cdklabs/aws-lambda-rust';
import { Duration } from 'aws-cdk-lib';
import { ITable } from 'aws-cdk-lib/aws-dynamodb';
import { Role, ServicePrincipal } from 'aws-cdk-lib/aws-iam';
import { Architecture, DockerImageCode, DockerImageFunction, LayerVersion, StartingPosition } from 'aws-cdk-lib/aws-lambda';
import { DynamoEventSource, SqsEventSource } from 'aws-cdk-lib/aws-lambda-event-sources';
import { CfnPipe } from 'aws-cdk-lib/aws-pipes';
import { IBucket } from 'aws-cdk-lib/aws-s3';
import { Queue } from 'aws-cdk-lib/aws-sqs';
import { StringParameter } from 'aws-cdk-lib/aws-ssm';
import { Construct } from 'constructs';

export interface SendNewsletterProcessingStackProps {
    newsletterTable: ITable,
    newsletterStorageBucket: IBucket,
    configParameter: StringParameter
}

export class SendNewsletterProcessingStack extends Construct {

  constructor(scope: Construct, id: string, props: SendNewsletterProcessingStackProps) {
    super(scope, id);

    const sendNewsletterQueue = new Queue(this, "SendNewsletterQueue");

    var pipeRole = new Role(this, "PipeIntegrationRole", {
        assumedBy: new ServicePrincipal("pipes.amazonaws.com")
      });
  
      props.newsletterTable.grantStreamRead(pipeRole);
      sendNewsletterQueue.grantSendMessages(pipeRole);
  
      var pipe = new CfnPipe(this, "NewSubscriberPipe", {
        roleArn: pipeRole.roleArn,
        source: (props.newsletterTable.tableStreamArn ?? ""),
        sourceParameters: {
          dynamoDbStreamParameters: {
            startingPosition: "TRIM_HORIZON",
            batchSize: 10
          },
          filterCriteria: {
            filters: [{
              pattern: '{"dynamodb.NewImage.Type.S": ["NewsletterIssue"]}'
            }]
          },
        },
        target: sendNewsletterQueue.queueArn,
        targetParameters: {
          sqsQueueParameters:{
  
          },
          inputTemplate: `{
            "trace_parent": <$.dynamodb.NewImage.TraceParent.S>,
            "parent_span": <$.dynamodb.NewImage.ParentSpan.S>,
            "issue_title": <$.dynamodb.NewImage.IssueTitle.S>,
            "s3_pointer": <$.dynamodb.NewImage.S3Pointer.S>
          }`
        }
      })
  
      const send_newsletter_function = new RustFunction(this, "NewsletterFunction", {
        entry: '../src/Cargo.toml',
        functionName: 'Zero2ProdSendNewsletterFunction',
        binaryName: 'send_newsletter',
        timeout: Duration.seconds(60),
        environment: {
          "APP_DATABASE__DATABASE_NAME": props.newsletterTable.tableName,
          "APP_DATABASE__NEWSLETTER_STORAGE_BUCKET": props.newsletterStorageBucket.bucketName,
          LOG_LEVEL: "error",
          CONFIG_PARAMETER_NAME: props.configParameter.parameterName,
          APP_ENVIRONMENT: "production",
          DD_OTLP_CONFIG_RECEIVER_PROTOCOLS_HTTP_ENDPOINT: "localhost:4318",
          AWS_LAMBDA_EXEC_WRAPPER: "/opt/datadog_wrapper",
          DD_SITE: "datadoghq.eu",
          DD_API_KEY: process.env.DATADOG_API_KEY ?? "",
          DD_ENV: "production",
          DD_SERVICE: "zero2prod-send-newsletter"
        },
        layers: [
          LayerVersion.fromLayerVersionArn(this, "DDExtension", "arn:aws:lambda:eu-west-1:464622532012:layer:Datadog-Extension-ARM:55")
        ],
        architecture: Architecture.ARM_64,
      });
  
      send_newsletter_function.addEventSource(new SqsEventSource(sendNewsletterQueue, {
        batchSize: 10
      }));
  
      props.newsletterStorageBucket.grantRead(send_newsletter_function);
      props.newsletterTable.grantReadData(send_newsletter_function);
      props.configParameter.grantRead(send_newsletter_function);
}
}
