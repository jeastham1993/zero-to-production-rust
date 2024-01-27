import * as cdk from 'aws-cdk-lib';
import { Construct } from 'constructs';
import { NewsletterApi } from './api-stack';
import { NewSubscriberProcessingStack } from './new-subscriber-stack';
import { SendNewsletterProcessingStack } from './send-newsletter-stack';

export class Zero2ProdApplicationStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);
    const api = new NewsletterApi(this, "NewsletterApi");
    
    const newSubscriberProcessor = new NewSubscriberProcessingStack(this, "NewSubscriberProcessor", {
      newsletterTable: api.NewsletterTable,
      newsletterStorageBucket: api.NewsletterStorageBucket
    });
    
    const sendNewsletterProcessor = new SendNewsletterProcessingStack(this, "SendNewsletterProcessor", {
      newsletterTable: api.NewsletterTable,
      newsletterStorageBucket: api.NewsletterStorageBucket
    });
  }
}
