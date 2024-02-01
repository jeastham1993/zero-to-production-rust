import { RustFunction } from '@cdklabs/aws-lambda-rust';
import { ITable } from 'aws-cdk-lib/aws-dynamodb';
import { Role, ServicePrincipal } from 'aws-cdk-lib/aws-iam';
import { Architecture, DockerImageCode, DockerImageFunction, StartingPosition } from 'aws-cdk-lib/aws-lambda';
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
        entry: '../src/backend/Cargo.toml',
        functionName: 'Zero2ProdSendNewsletterFunction',
        binaryName: 'send_newsletter',
        environment: {
          "APP_DATABASE__DATABASE_NAME": props.newsletterTable.tableName,
          "APP_DATABASE__NEWSLETTER_STORAGE_BUCKET": props.newsletterStorageBucket.bucketName,
          LOG_LEVEL: "error",
          CONFIG_PARAMETER_NAME: props.configParameter.parameterName,
          APP_ENVIRONMENT: "production"
        },
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
